Dir.glob(Rails.root.join("**/#{path}/*.rb"))
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ `Rails.root` is a `Pathname`, so you can use `Rails.root.glob("**/#{path}/*.rb")`.
