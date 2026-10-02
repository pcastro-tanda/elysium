permitted_keys = %i[name age]
params.require(:user).permit(permitted_keys)
