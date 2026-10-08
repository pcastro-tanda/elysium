(foo || x.to_i) || fallback
                ^^^^^^^^^^^ `fallback` will never evaluate because `x.to_i` always returns a truthy value.
