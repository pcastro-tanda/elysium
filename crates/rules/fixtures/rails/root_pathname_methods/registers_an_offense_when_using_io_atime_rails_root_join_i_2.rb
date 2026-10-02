::IO.atime(::Rails.root.join('db', 'schema.rb'), 20, 5)
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ `::Rails.root` is a `Pathname`, so you can use `::Rails.root.join('db', 'schema.rb').atime(20, 5)`.
