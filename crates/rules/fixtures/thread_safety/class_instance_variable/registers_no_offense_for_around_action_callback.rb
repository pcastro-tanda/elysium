def self.foo
  around_action do
    @language = :haskell
  end
end
