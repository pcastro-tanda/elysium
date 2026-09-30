@objects = @objects.where(type: :a)

@objects = @objects
  .with_relation
  .paginate
