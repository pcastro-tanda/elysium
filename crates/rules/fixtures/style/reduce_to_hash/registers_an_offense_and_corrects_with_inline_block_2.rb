array.inject({}) { |hash, elem| hash[elem.id] = elem.name; hash }
      ^^^^^^ Use `to_h { ... }` instead of `inject`.
