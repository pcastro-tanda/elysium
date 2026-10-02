files.map { |file| File.open(Rails.root.join('db', file), 'wb') }
                   ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ `Rails.root` is a `Pathname`, so you can use `Rails.root.join('db', file).open('wb')`.
