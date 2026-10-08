foo.each_with_object({}) do |f, hash|
  changes = hash.merge!(a: 1, b: 2)
  why_are_you_doing_this?
end
