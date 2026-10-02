Dir.each_child(Rails.public_path)
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ `Rails.public_path` is a `Pathname`, so you can use `Rails.public_path.each_child`.
