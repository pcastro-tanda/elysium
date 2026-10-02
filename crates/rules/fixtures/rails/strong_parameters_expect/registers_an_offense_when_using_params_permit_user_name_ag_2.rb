params&.permit(user: [:name, :age])&.require(:user)
        ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `expect(user: [:name, :age])` instead.
