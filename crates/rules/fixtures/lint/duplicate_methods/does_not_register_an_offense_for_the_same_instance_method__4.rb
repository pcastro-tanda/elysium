T.cast(
  Class.new(Base) do
    def perform
      1
    end
  end,
  T.class_of(Base)
)

T.cast(
  Class.new(Base) do
    def perform
      2
    end
  end,
  T.class_of(Base)
)
