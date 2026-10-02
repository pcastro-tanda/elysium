File.join(Rails.root, %w[app models])
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ `Rails.root` is a `Pathname`, so you can use `Rails.root.join(*%w[app models])`.
