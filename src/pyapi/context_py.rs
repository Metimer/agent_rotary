use pyo3::prelude::*;
use pyo3::types::{PyDict, PyInt, PyList};
use pyo3::IntoPyObjectExt;
use serde_json::Value;

use crate::context::Context;

/// Sac de données dynamique partagé entre les nodes, exposé à Python.
/// Mapping bidirectionnel avec `dict` Python via `serde_json::Value`.
#[pyclass(name = "Context", skip_from_py_object)]
#[derive(Clone)]
pub struct PyContext {
    pub inner: Context,
}

#[pymethods]
impl PyContext {
    #[new]
    #[pyo3(signature = (data=None))]
    fn new(data: Option<Bound<'_, PyDict>>) -> PyResult<Self> {
        let mut ctx = Context::new();
        if let Some(d) = data {
            for (k, v) in d.iter() {
                let key: String = k.extract()?;
                let val = py_to_value(&v)?;
                ctx.set(key, val);
            }
        }
        Ok(PyContext { inner: ctx })
    }

    /// Récupère une valeur convertie vers un type natif Python.
    /// `get(key, default)` renvoie `default` (ou None) si absent.
    #[pyo3(signature = (key, default=None))]
    fn get(&self, py: Python<'_>, key: &str, default: Option<Py<PyAny>>) -> PyResult<Py<PyAny>> {
        match self.inner.get(key) {
            Some(v) => value_to_py(py, v),
            None => match default {
                Some(d) => Ok(d),
                None => Ok(py.None()),
            },
        }
    }

    /// Accès numérique commode (note, score) avec défaut.
    #[pyo3(signature = (key, default=0.0))]
    fn get_number(&self, key: &str, default: f64) -> f64 {
        self.inner.get_number(key).unwrap_or(default)
    }

    fn set(&mut self, key: &str, value: Bound<'_, PyAny>) -> PyResult<()> {
        let v = py_to_value(&value)?;
        self.inner.set(key, v);
        Ok(())
    }

    fn __getitem__(&self, py: Python<'_>, key: &str) -> PyResult<Py<PyAny>> {
        match self.inner.get(key) {
            Some(v) => value_to_py(py, v),
            None => Err(pyo3::exceptions::PyKeyError::new_err(key.to_string())),
        }
    }

    fn __setitem__(&mut self, key: &str, value: Bound<'_, PyAny>) -> PyResult<()> {
        let v = py_to_value(&value)?;
        self.inner.set(key, v);
        Ok(())
    }

    fn __contains__(&self, key: &str) -> bool {
        self.inner.get(key).is_some()
    }

    fn __repr__(&self) -> String {
        let entries: Vec<String> = self
            .inner
            .snapshot()
            .iter()
            .map(|(k, v)| format!("{k:?}: {v}"))
            .collect();
        format!("Context({{{}}})", entries.join(", "))
    }
}

impl From<Context> for PyContext {
    fn from(c: Context) -> Self {
        PyContext { inner: c }
    }
}

/// Convertit une valeur Python vers `serde_json::Value`.
fn py_to_value(obj: &Bound<'_, PyAny>) -> PyResult<Value> {
    if obj.is_none() {
        Ok(Value::Null)
    } else if let Ok(b) = obj.extract::<bool>() {
        Ok(Value::Bool(b))
    } else if let Ok(i) = obj.extract::<i64>() {
        Ok(serde_json::Number::from(i).into())
    } else if let Ok(i) = obj.extract::<u64>() {
        Ok(serde_json::Number::from(i).into())
    } else if obj.is_instance_of::<PyInt>() {
        Err(pyo3::exceptions::PyOverflowError::new_err(
            "integer is outside the JSON i64 or u64 range",
        ))
    } else if let Ok(f) = obj.extract::<f64>() {
        serde_json::Number::from_f64(f)
            .map(Value::Number)
            .ok_or_else(|| pyo3::exceptions::PyValueError::new_err("invalid float"))
    } else if let Ok(s) = obj.extract::<String>() {
        Ok(Value::String(s))
    } else if let Ok(d) = obj.cast::<PyDict>() {
        let mut map = serde_json::Map::new();
        for (k, v) in d.iter() {
            let key: String = k.extract()?;
            map.insert(key, py_to_value(&v)?);
        }
        Ok(Value::Object(map))
    } else if let Ok(l) = obj.cast::<PyList>() {
        let mut arr = Vec::new();
        for item in l.iter() {
            arr.push(py_to_value(&item)?);
        }
        Ok(Value::Array(arr))
    } else {
        Ok(Value::String(obj.str()?.to_string_lossy().to_string()))
    }
}

/// Convertit une `serde_json::Value` vers un objet Python possédé (`Py<PyAny>`).
fn value_to_py(py: Python<'_>, v: &Value) -> PyResult<Py<PyAny>> {
    match v {
        Value::Null => Ok(py.None()),
        Value::Bool(b) => b.into_py_any(py),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                i.into_py_any(py)
            } else if let Some(i) = n.as_u64() {
                i.into_py_any(py)
            } else {
                n.as_f64().unwrap_or(0.0).into_py_any(py)
            }
        }
        Value::String(s) => s.clone().into_py_any(py),
        Value::Array(arr) => {
            let items: Vec<Py<PyAny>> = arr
                .iter()
                .map(|v| value_to_py(py, v))
                .collect::<PyResult<_>>()?;
            items.into_py_any(py)
        }
        Value::Object(map) => {
            let dict = PyDict::new(py);
            for (k, val) in map {
                dict.set_item(k, value_to_py(py, val)?)?;
            }
            Ok(dict.unbind().into_any())
        }
    }
}
