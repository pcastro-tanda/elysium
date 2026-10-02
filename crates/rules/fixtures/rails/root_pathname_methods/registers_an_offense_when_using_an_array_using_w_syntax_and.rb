File.join(Rails.root, %w[app models], "goober")
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ `Rails.root` is a `Pathname`, so you can use `Rails.root.join(*%w[app models], "goober")`.
