files.map { |file| Rails.root.join('db', file).open('wb') }
