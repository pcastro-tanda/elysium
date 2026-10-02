File.join(Rails.root, ['app', 'models'])
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ `Rails.root` is a `Pathname`, so you can use `Rails.root.join(*['app', 'models'])`.
