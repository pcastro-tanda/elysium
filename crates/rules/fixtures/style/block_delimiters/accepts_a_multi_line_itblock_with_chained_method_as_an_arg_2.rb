foo :bar, baz: 'Some text: %s' %
               %w[foo bar].map {
                it.upcase
               }.join(', ')
