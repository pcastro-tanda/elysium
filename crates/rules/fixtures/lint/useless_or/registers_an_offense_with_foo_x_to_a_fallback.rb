foo || x.to_a || fallback
              ^^^^^^^^^^^ `fallback` will never evaluate because `x.to_a` always returns a truthy value.
