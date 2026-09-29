a, (_, (b, _), *_) = foo()
              ^^^ Do not use trailing `_`s in parallel assignment. Prefer `a, (_, (b, _),) = foo()`.
          ^^ Do not use trailing `_`s in parallel assignment. Prefer `a, (_, (b,), *_) = foo()`.
