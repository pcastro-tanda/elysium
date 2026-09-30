@objects = @objects.where(type: :a)

@objects = @objects.
                   ^ Place the . on the next line, together with the method name.
  with_relation.
               ^ Place the . on the next line, together with the method name.
  paginate
