[a, b].map { _1.call 'something' }.uniq.join(' - ')
