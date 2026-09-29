foo :bar, baz: 'Some text: %s' %
               %w[foo bar].map { |v|
                v.upcase
               }.join(', ')
