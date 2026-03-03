package com.ztecompanion.core.di

import android.content.Context
import com.ztecompanion.core.network.AuthManager
import com.ztecompanion.core.network.UbusClient
import dagger.Module
import dagger.Provides
import dagger.hilt.InstallIn
import dagger.hilt.android.qualifiers.ApplicationContext
import dagger.hilt.components.SingletonComponent
import javax.inject.Singleton

@Module
@InstallIn(SingletonComponent::class)
object AppModule {

    @Provides
    @Singleton
    fun provideUbusClient(): UbusClient = UbusClient()

    @Provides
    @Singleton
    fun provideAuthManager(
        @ApplicationContext context: Context,
        ubusClient: UbusClient,
    ): AuthManager = AuthManager(context, ubusClient)
}
