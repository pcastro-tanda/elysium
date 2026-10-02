var = :user
params.require(var).permit(:name, some_ids: [])
       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `expect(var => [:name, some_ids: []])` instead.
