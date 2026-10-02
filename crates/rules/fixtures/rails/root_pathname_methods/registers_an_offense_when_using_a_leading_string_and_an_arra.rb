File.join(Rails.root, "app", ["models", "goober"])
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ `Rails.root` is a `Pathname`, so you can use `Rails.root.join("app", *["models", "goober"])`.
