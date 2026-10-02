Class.new(Resolvers::BaseResolver).tap do |c|
  c.const_set('MODEL_CLASS', model_class)
end
