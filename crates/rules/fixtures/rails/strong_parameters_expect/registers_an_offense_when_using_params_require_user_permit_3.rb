params.require(:user).permit(:name, some_ids: [])
       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `expect(user: [:name, some_ids: []])` instead.
