def self.foo
  after_action do
    @language = :haskell
  end
end
