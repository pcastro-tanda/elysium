File.join(Rails.root, "app", %w[models goober])
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ `Rails.root` is a `Pathname`, so you can use `Rails.root.join("app", *%w[models goober])`.
