x.__id__ || fallback || other_fallback
         ^^^^^^^^^^^ `fallback` will never evaluate because `x.__id__` always returns a truthy value.
