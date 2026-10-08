x.intern || fallback || other_fallback
         ^^^^^^^^^^^ `fallback` will never evaluate because `x.intern` always returns a truthy value.
