::IO.executable_real?(::Rails.root.join('db', 'schema.rb'))
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ `::Rails.root` is a `Pathname`, so you can use `::Rails.root.join('db', 'schema.rb').executable_real?`.
