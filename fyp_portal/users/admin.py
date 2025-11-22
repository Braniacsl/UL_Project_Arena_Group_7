from django.contrib import admin
from django.contrib.auth.admin import UserAdmin
from .models import User, Supervisor


# Register your models here.

@admin.register(User)
class CustomUserAdmin(admin.ModelAdmin):
    list_display = ['username', 'email', 'role', 'is_staff', 'created_at']
    list_filter = ['role','is_staff','is_active']
    fieldsets = UserAdmin.fieldsets + (
        ('Additional Info', {'fields': ('role','created_at')}),
    )
    readonly_fields = ['created_at']


@admin.register(Supervisor)
class SupervisorAdmin(admin.ModelAdmin):
    list_display = ['name','email','department']
    search_fields = ['name','email','department']
