(foo || x.intern) || fallback
                  ^^^^^^^^^^^ `fallback` will never evaluate because `x.intern` always returns a truthy value.
