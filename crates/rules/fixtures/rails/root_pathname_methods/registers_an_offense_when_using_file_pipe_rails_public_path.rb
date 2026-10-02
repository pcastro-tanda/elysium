File.pipe?(Rails.public_path)
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ `Rails.public_path` is a `Pathname`, so you can use `Rails.public_path.pipe?`.
