T.cast(Class.new(Base) { def perform; 1; end }, T.class_of(Base)); T.cast(Class.new(Base) { def perform; 2; end }, T.class_of(Base))
