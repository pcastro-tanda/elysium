x ||= begin
        1
      rescue
      ^^^^^^ `rescue` at 3, 6 is not aligned with `x ||= begin` at 1, 0.
        2
      end
