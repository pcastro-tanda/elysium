File.expand_path(Rails.public_path)
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ `Rails.public_path` is a `Pathname`, so you can use `Rails.public_path.expand_path`.
