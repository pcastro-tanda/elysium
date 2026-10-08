def self.foo
  prepend_before_action do
    @language = :haskell
  end
end
