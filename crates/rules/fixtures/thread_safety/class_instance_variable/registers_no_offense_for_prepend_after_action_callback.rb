def self.foo
  prepend_after_action do
    @language = :haskell
  end
end
