params&.require(:user)&.permit(:name, :age)
        ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `expect(user: [:name, :age])` instead.
