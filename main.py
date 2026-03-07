import os
import discord
from discord.ext import commands
from dotenv import load_dotenv

async def trigger_orichalcum_ingestion(message, author):
    print("YAY")

def main():
    load_dotenv()
    TOKEN = os.getenv('DISCORD_TOKEN')
    TARGET_CHANNEL_NAME  = os.getenv('TARGET_CHANNEL_NAME') or "dropbox"
    if not TOKEN:
        print("Error: DISCORD_TOKEN was not found, please ensure to set it up in the environment")
        return -1

    # Define intents
    intents = discord.Intents.default()
    intents.message_content = True  
    intents.members = True

    # Define the commands for the bot
    bot = commands.Bot(command_prefix="!", intents=intents)

    # This is just for us in the terminal
    @bot.event
    async def on_ready():
        print(f"Success: {bot.user} is connected and listening!")

    # This is the automation trigger for listening on channel
    @bot.event
    async def on_message(message):
        if message.author == bot.user:
            # the bot doesn't react to its own messages
            return

        if message.channel.name == TARGET_CHANNEL_NAME:
            await trigger_orichalcum_ingestion(message.content, message.author)
            await message.add_reaction('🧠')

        await bot.process_commands(message)

    # Command handler for a 'ping' command
    @bot.command()
    async def ping(ctx):
        await ctx.send('Pong!')

    bot.run(TOKEN)    



if __name__ == "__main__":
    main()
