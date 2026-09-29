arr&.each do |x|
          ^^ Prefer `{...}` over `do...end` for multi-line chained blocks.
end&.map(&:to_s)
