user = authorize scope.includes(:user)
                      .where(name: 'Bob')
                      .order(:name)
