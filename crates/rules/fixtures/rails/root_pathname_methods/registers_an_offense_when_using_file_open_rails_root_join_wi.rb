file = File.open Rails.root.join 'docs', 'invoice.pdf'
       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ `Rails.root` is a `Pathname`, so you can use `Rails.root.join('docs', 'invoice.pdf').open`.
