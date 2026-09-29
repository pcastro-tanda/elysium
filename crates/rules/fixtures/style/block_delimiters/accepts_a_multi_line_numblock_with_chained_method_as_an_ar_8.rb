foo :bar, baz: 'Some text: %s' %
               %w[foo bar].map {
                _1.upcase
               }.join(', ')
