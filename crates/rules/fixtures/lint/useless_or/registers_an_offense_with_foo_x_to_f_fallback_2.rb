(foo || x.to_f) || fallback
                ^^^^^^^^^^^ `fallback` will never evaluate because `x.to_f` always returns a truthy value.
