Dir.glob(Rails.root.join('**', '*.rb'))
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ `Rails.root` is a `Pathname`, so you can use `Rails.root.glob("**/*.rb")`.
