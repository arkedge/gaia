use anyhow::{anyhow, ensure, Result};

pub mod schema;

pub struct Writer<'b, I> {
    iter: I,
    static_size: usize,
    has_trailer: bool,
    bytes: &'b mut [u8],
}

impl<'b, I> Writer<'b, I> {
    pub fn new(iter: I, static_size: usize, has_trailer: bool, bytes: &'b mut [u8]) -> Self {
        Self {
            iter,
            static_size,
            has_trailer,
            bytes,
        }
    }
}

impl<'a, 'b, I, F> Writer<'b, I>
where
    I: Iterator<Item = &'a F>,
    F: structpack::SizedField + 'static,
{
    pub fn write(&mut self, value: F::Value<'_>) -> Result<()> {
        let field = self
            .iter
            .next()
            .ok_or_else(|| anyhow!("all fields have been written"))?;
        field.write(self.bytes, value)?;
        Ok(())
    }

    fn verify_completion(&mut self) -> Result<()> {
        ensure!(
            self.iter.next().is_none(),
            "some parameters have not been written yet"
        );
        Ok(())
    }

    /// Returns total (static + trailer) len
    pub fn write_trailer_and_finish(mut self, trailer: &[u8]) -> Result<usize> {
        ensure!(
            self.has_trailer,
            "this command has no trailer(RAW) parameter"
        );
        self.verify_completion()?;
        let buf = self
            .bytes
            .get_mut(self.static_size..self.static_size + trailer.len())
            .ok_or_else(|| anyhow!("trailer is too long for the buffer"))?;
        buf.copy_from_slice(trailer);
        Ok(self.static_size + trailer.len())
    }

    pub fn finish(mut self) -> Result<usize> {
        self.verify_completion()?;
        Ok(self.static_size)
    }
}
