Dir[Rails.root.join('spec/support/**/*.rb')]
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ `Rails.root` is a `Pathname`, so you can use `Rails.root.glob('spec/support/**/*.rb')`.
