x.inspect || fallback || other_fallback
          ^^^^^^^^^^^ `fallback` will never evaluate because `x.inspect` always returns a truthy value.
