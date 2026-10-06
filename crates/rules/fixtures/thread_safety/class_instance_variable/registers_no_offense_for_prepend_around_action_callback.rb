def self.foo
  prepend_around_action do
    @language = :haskell
  end
end
