x.to_r || fallback
       ^^^^^^^^^^^ `fallback` will never evaluate because `x.to_r` always returns a truthy value.
