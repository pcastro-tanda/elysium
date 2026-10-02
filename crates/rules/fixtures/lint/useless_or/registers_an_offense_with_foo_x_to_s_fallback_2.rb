(foo || x.to_s) || fallback
                ^^^^^^^^^^^ `fallback` will never evaluate because `x.to_s` always returns a truthy value.
