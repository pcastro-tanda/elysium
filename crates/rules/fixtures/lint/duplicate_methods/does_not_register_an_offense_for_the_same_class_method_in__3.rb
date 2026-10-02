T.cast(
  Class.new(Base) do
    def self.perform
      1
    end
  end,
  T.class_of(Base)
)

T.cast(
  Class.new(Base) do
    def self.perform
      2
    end
  end,
  T.class_of(Base)
)
