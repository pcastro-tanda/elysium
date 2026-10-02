params.require(:user).permit(*parameters, some_ids: [])
       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `expect(user: [*parameters, some_ids: []])` instead.
