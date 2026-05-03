/// Erreurs de codegen Cranelift

#[derive(Debug)]
pub struct CodegenError(pub String);

impl std::fmt::Display for CodegenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "codegen error: {}", self.0)
    }
}

impl std::error::Error for CodegenError {}

pub type CgResult<T> = Result<T, CodegenError>;
